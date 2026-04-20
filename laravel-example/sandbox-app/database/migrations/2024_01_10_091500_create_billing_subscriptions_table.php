<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

class CreateBillingSubscriptionsTable extends Migration
{
    public function up(): void
    {
        Schema::create('billing_subscriptions', function (Blueprint $table) {
            $table->id();
            $table->foreignId('user_id')->references('id')->on('users');
            $table->string('plan_code');
            $table->enum('status', ['trialing', 'active', 'past_due', 'canceled'])->default('trialing');
            $table->decimal('unit_amount')->default(0);
            $table->string('currency')->default('USD');
            $table->unsignedInteger('seat_count')->default(1);
            $table->string('legacy_plan_code')->nullable();
            $table->json('metadata')->nullable();
            $table->timestamp('trial_ends_at')->nullable();
            $table->timestamps();
        });
    }
}
