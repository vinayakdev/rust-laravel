<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('billing_subscriptions', function (Blueprint $table) {
            $table->string('provider_subscription_id')->nullable()->unique();
            $table->timestamp('ends_at')->nullable();
        });
    }
};
